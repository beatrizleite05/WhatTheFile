# Technical Decisions

Single source of truth for every architecture decision locked into v1. Each entry states the decision, the evidence behind it (benchmark, audit, or recommendation), and any known follow-ups.

A decision marked **`provisional`** is locked in intent but pending validation by the eval harness during Phase D rework. If the eval shows regression, the decision is revisited before Phase D exits.

---

## Locked Stack (summary)

| Area | Decision |
|---|---|
| Desktop shell | Tauri |
| Indexing strategy | Hybrid — fast metadata pass + blake3 fingerprint fallback |
| Background runtime | Index on app open (incremental); background service is post-v1 |
| Chunking | 512-token window, 64-token overlap (provisional) |
| Ranking | RRF in Rust (`search.rs`); display-layer sort only in `ranking.ts` |
| Embedding model | Model-agnostic config; `nomic-embed-text-v2-moe` (768-dim) is the v1 default |
| Embedding provider | Ollama — text via `nomic-embed-text-v2-moe`; images via `qwen2.5vl:7b` (fallback `llava:7b`) |
| Spreadsheet parsing | Flat normalisation (file-level); sheet-aware is post-v1 |
| OCR | Confidence threshold `0.75` (PDF text-layer fallback path) |
| Multilingual | FTS5 accent folding + native cross-lingual embeddings; no translation dictionary |
| Privacy UX | Explicit controls — scope editor, audit log, delete index |
| Duplicate presentation | Listed separately |
| Reranker (v1) | None |
| Eval harness | In v1; native Rust binary; results committed |
| Test layering | CI = unit/types/lint only. Local pre-push gate (Mac) runs the full suite + eval against live Ollama |

---

## 1. Desktop Shell — Tauri

Selected over Electron for v1.

- Lower RAM/CPU overhead for always-on/background indexing workflows.
- Smaller distribution footprint.
- Strong fit for lightweight cross-platform desktop delivery (macOS + Windows).
- Trade-off: more Rust-side integration work; tighter process supervision.

Validation gate: macOS and Windows build/package flow works end-to-end; file picker + tray/background behaviour acceptable. Re-evaluate Electron only on a critical blocker.

---

## 2. Indexing Strategy — Hybrid-balanced

Event-driven file watching + periodic reconciliation. Locked after benchmark against polling-only.

| Strategy | Mean freshness | p95 freshness | Missed changes | CPU units |
|---|---|---|---|---|
| Polling-only | 92 ms | 110 ms | 0 | 650 |
| Hybrid-balanced | 27.7 ms | 38 ms | 0 | 436 |

Hybrid is 3× fresher and 33% cheaper.

---

## 3. Background Runtime — Incremental indexing on app open (v1)

Index runs inside the Tauri process, triggered on app open. No separate background service.

| Model | Freshness after close | Closed-window coverage | Complexity |
|---|---|---|---|
| Window-only | 270125 ms mean | 0% | 2 |
| Tray-resident | 80 ms mean | 100% | 5 |
| Background-helper | 60 ms mean | 100% | 8 |

The freshness gap is acceptable for personal use — users rarely add a file and immediately search before opening the app. Background-helper adds permanent RAM overhead, platform service registration, silent failure risk, and user-visible background processes — friction that conflicts with local-first/privacy-first values.

**Roadmap:** promote to background-helper if stale results become a real pain point. Implementation work then required: service lifecycle, IPC contract (recommend SQLite as shared state bus + polling), reconnect behaviour, sleep/wake revalidation, install/update strategy per platform.

---

## 4. Chunking — 512 / 64 (provisional)

**v1 default:** 512-token window, 64-token overlap.

Original benchmark (256/32) was conservative and produced fragmented semantic units for prose. Reference-repo audit (Vexor, aiwhispr) and `nomic-embed-text-v2-moe`'s 8192-token context support a larger window. 512 doubles per-chunk semantic density without exploding storage; 64-token overlap (~12.5%) preserves boundary continuity.

Constants live in `chunker.rs` and `chunker.ts` and are not user-configurable.

**Provisional:** locked in intent. Validated by the Phase D eval harness against the previous 256/32 baseline. If the eval shows regression on the curated corpus, revisit window size before Phase D exits.

---

## 5. Ranking — Hybrid RRF

| Strategy | MRR | Recall@1 |
|---|---|---|
| Semantic-only | 0.50 | 0.00 |
| Hybrid | 1.00 | 1.00 |

Exact and path-like queries fail with semantic-only. Hybrid is the clearest benchmark win.

```
score = 1 / (k + rank_fts) + 1 / (k + rank_vec)      with k = 60
```

RRF is preferred over weighted-average blending: no score normalisation required, degrades gracefully when one signal is absent (file with no extractable text → no BM25 rank → RRF still produces a result).

**v1:** RRF unchanged, equal weights, k=60. Score-weighted blends and auto-mode-pick deferred post-v1 pending eval-driven tuning.

**User-selectable modes** (`SearchRequest.mode`): `hybrid` (default) | `keyword` | `semantic`.

---

## 6. Embedding Provider & Model — Ollama / nomic-embed-text-v2-moe (768-dim)

| Strategy | Model | NDCG@3 | Recall@3 | MRR | External transfer | Cost |
|---|---|---|---|---|---|---|
| `nomic-local` | nomic-embed-text-v1.5 + nomic-embed-vision-v1.5 | 0.596 | 0.647 | 0.617 | 0 bytes | $0 |
| `gemini-cloud` | gemini-embedding-2-preview | 0.736 | 0.804 | 0.745 | all content | $0.20/M tokens |

Gemini leads by ~14 NDCG@3 points. Gap accepted for a local-first product.

**Decision:** Ollama local stack.
- Text/document embeddings: `nomic-embed-text-v2-moe` (768-dim by architecture; not configurable).
- Image understanding: `qwen2.5vl:7b` (fallback `llava:7b`) describes the image → description embedded with the text model.
- One Ollama process handles both; no Python microservice, no ONNX runtime.

`qwen2.5vl:7b` outperforms llava on text-heavy images (receipts, forms, scans), the dominant personal-file use case. Fallback `llava:7b` for machines that cannot load qwen (~5GB vs ~4GB).

Storage cost for 768-dim: ~3KB per chunk (768 × 4 bytes); acceptable at desktop scale.

The earlier `embedding-64` benchmark on a synthetic corpus is superseded by the model choice — `nomic-embed-text-v2-moe` outputs 768-dim natively.

---

## 7. Embedding Model Architecture — Model-agnostic

The retrieval algorithm must never assume nomic-specific behaviour. Per-model configuration captures everything that varies:

```rust
struct EmbeddingModel {
    name: &'static str,        // e.g. "nomic-embed-text-v2-moe"
    dim: usize,                // 768
    query_prefix: &'static str,   // "search_query: "
    doc_prefix:   &'static str,   // "search_document: "
}
```

Swapping models (e.g. to `bge-m3` post-v1) is a config change, not new architecture.

---

## 8. Embedding Normalisation — Unconditional L2 + cosine (provisional)

Every embedding is L2-normalised before storage and before query, regardless of whether the source model emits unit vectors. SQL uses `vec_distance_cosine` explicitly.

**Why:** `nomic-embed-text-v2-moe` does not always emit unit vectors. Defense-in-depth: works correctly for any future model regardless of its output norm.

**Provisional:** validated by Phase D eval (correctness + Recall@10 lift over baseline).

---

## 9. Query / Document Prefixes — Required (provisional)

`nomic-embed-text-v2-moe` was trained with task prefixes:
- Indexing: `"search_document: " + chunk_text`
- Querying: `"search_query: " + user_text`

Without prefixes, query and document embeddings live in different regions of vector space — cosine becomes near-noise. This is the most likely root cause of the "irrelevant semantic results" symptom that triggered the Phase D rework.

**Provisional:** validated by Phase D eval as the largest single-source relevance lift.

**Migration: embedding-only.** Bump `model_version` so chunks are detected as needing re-embedding. Extracted text and image descriptions are preserved — only `chunks_vec` rows are rebuilt. Avoids re-running OCR/vision on images.

---

## 10. Hybrid Blend (v1) — RRF unchanged

`k = 60`, equal weights. Reasons:
- Once prefixes (decision 9) are correct, vector ranks become reliable; RRF stops feeling flat.
- Weighted-sum requires score normalisation that drifts (BM25 IDF, cosine bounds).
- One un-measurable parameter is one parameter set wrong.
- RRF k=60 is the literature default and corpus-independent.

Score-weighted blends and auto-mode-pick (e.g. bias toward FTS for keyword-y queries) deferred post-v1 pending eval-driven tuning.

---

## 11. Reranker (v1) — None

Vexor / aiwhispr audit identifies cross-encoder reranking as a real lift, but:
- A reranker on top of a broken vector pipeline can't be evaluated.
- The Python-based FlashRank conflicts with the locked Ollama-only constraint.
- Native Rust ONNX rerankers add ~30-80MB to the bundle; Phase F packaging is already non-trivial.
- LLM-as-reranker via Ollama adds 1.5-3s per query — breaks the responsive-UX principle.

**v1:** ship the corrected hybrid retrieval (decisions 4, 7, 8, 9) and measure. Add a reranker only post-v1, only if eval shows residual quality gap.

### Post-v1 Reranker Roadmap

Before adding any reranker, build the eval harness (decision 22) and run baseline. Do not commit to a reranker without measurement.

**Option D — Stronger embedding model (recommended first step).** Swap to `bge-m3` (1024-dim, multilingual) or `mxbai-embed-large` via Ollama. Composes trivially with decision 7 — config swap, not architecture. ~33% storage increase; pt-BR support built-in.

**Option C — Native Rust cross-encoder (ONNX Runtime).** `bge-reranker-v2-m3` (~278MB) or `mmarco-mMiniLMv2-L12` (~120MB) via the `ort` crate. ~50-150ms total on CPU for top-20 candidates. Cons: per-platform native libs, +30-80MB bundle, complicates Phase F CI; ~100-300ms cold-start. Adopt only if measured residual gap after Option D is significant.

**Option B — LLM-as-reranker via Ollama (not recommended).** Score top-K via prompt to a small instruct model. 1.5-3s added latency; small LLMs are unreliable scalar scorers. Adopt only if Option C is genuinely infeasible AND latency can be hidden by progressive results.

**Option E — Learned-to-rank from implicit feedback.** LightGBM on `[fts_rank, vec_rank, recency, file_type, …]` with click/preview-dwell signals. Personalised; sub-1MB model. Cons: needs on-device telemetry, complex eval, cold-start. Phase 3+ territory.

**Sequence:** instrument eval → try Option D → measure → only then consider Option C. Skip B unless C is infeasible. Defer E indefinitely.

---

## 12. Spreadsheet Parsing — Flat normalisation

| Strategy | Precision@1 | NDCG@3 | Index size |
|---|---|---|---|
| Flat | 0.667 | 0.833 | 72 bytes |
| Sheet-aware | 1.000 | 1.000 | 208 bytes |

Sheet-aware wins the benchmark, but the app only needs document-level context for v1. Sheet hints add index size without benefit at this retrieval granularity.

**Follow-up:** revisit if sheet-level queries become common.

---

## 13. OCR Confidence Threshold — 0.75

Applies to the PDF text-layer-fallback OCR path (Tesseract / leptess).

| Strategy | Precision@1 | NDCG@3 | Index size |
|---|---|---|---|
| Threshold 0.2 | 0.50 | 0.815 | 152 bytes |
| Threshold 0.75 | 1.00 | 1.00 | 96 bytes |

Stricter threshold removes noisy OCR text and improves both precision and index footprint. Could hide faint true positives on harder corpora — acceptable for v1.

The image path uses vision-LLM description (decision 15) and does not apply this threshold; vision results carry a fixed `VISION_CONFIDENCE` constant.

---

## 14. Multilingual — Accent folding + cross-lingual embeddings

**Accent/diacritic variation** (`relatório` → `relatorio`) is handled natively by FTS5's `unicode61 remove_diacritics 2` tokenizer.

**Cross-language matching** (Portuguese query against an English file) is handled by the multilingual embedding model — once query/document prefixes (decision 9) are correct, semantically-equivalent text lands close in vector space.

**Language detection at index time** (`whatlang` crate): stored as `lang_hint` on `files` (e.g. `'pt'`, `'en'`, `'unknown'`); informational only in v1. Hybrid blend weights stay fixed regardless of language match.

**Translation dictionary: dropped.** The earlier benchmark showing a gap for locale-aware normalisation was FTS-only; in hybrid search with multilingual embeddings, the semantic path covers it without dictionary maintenance cost.

**Roadmap:** use `lang_hint` to weight FTS vs. semantic when cross-language queries become a measured pain point. Field is in the schema; logic is not implemented.

**FTS5 query escaping (follow-up).** `fts5_escape` in `search.rs` wraps the entire user query in double-quotes, making FTS5 treat it as a phrase search. This prevents hyphens and accented tokens from being parsed as column references or operators (e.g. `deixe-se` would otherwise be interpreted as `column_deixe MINUS se`). The tradeoff is that explicit `AND`/`OR`/`NOT` operators typed by the user are no longer honoured by FTS5 (they're treated as literal words). This is acceptable in v1 because the hybrid path handles relevance; operator support is a post-v1 improvement if user research shows demand.

---

## 15. Vision Pipeline — Determinism + unified prompt (provisional)

**Determinism.** All `qwen2.5vl` calls send `"options": { "temperature": 0 }`. Identical images produce identical descriptions across reindex.

**Unified structured prompt.** Single call per image producing four fields:

```
SUBJECTS: <main subjects/objects>
SCENE: <setting and context>
VISUAL_DETAILS: <colors, style, notable elements>
TEXT: <transcribe ALL visible text verbatim, preserving order. Write 'NONE' if no text is visible.>
```

The model self-decides whether OCR-style transcription is warranted. No separate OCR pass for pure photos. Single model in VRAM.

`TEXT: NONE` sentinel prevents hallucinated text on textless images.

**Provisional:** validated by Phase D eval against the curated image corpus.

---

## 16. Snippet Source Selection (provisional)

The displayed snippet is chosen per-file from whichever pass ranked it higher:
- **FTS-leading file:** FTS5 `snippet()` window centred on the matched term.
- **Vector-leading file:** first sentence(s) of the best-scoring chunk.

Avoids the "result has no visible match" perception that makes correct rankings *feel* irrelevant.

**Provisional:** validated by Phase D eval (perceived-relevance metric still TBD; structural change).

---

## 17. Snippet Highlighting — Safe-by-construction `<mark>`

Snippet payload contains `<mark>...</mark>` markers end-to-end.

**Server-side construction:**
1. Escape chunk text first (HTML entities).
2. For FTS5 path: `snippet()` uses non-printable delimiters (e.g. `\x01...\x02`); escape the surrounding text, then convert delimiters → `<mark>...</mark>`.
3. For vector path: escape, then wrap matched terms (case-insensitive, whole-token) in `<mark>...</mark>`.

**Frontend rendering:** a small purpose-built splitter parses `<mark>` and emits React elements. Never raw-HTML injection. No DOMPurify dependency.

---

## 18. v1 File-Type Scope

`pdf`, `docx`, `xlsx`, `xls`, `xlsm`, `csv`, `txt`, `md`, `png`, `jpg/jpeg`, `webp`.

`.xls` and `.xlsm` route through the existing `calamine`-based XLSX extractor (already supported). `.webp` routes through the existing image vision pipeline. `.pptx` remains out of scope.

---

## 19. Query Parser — LLM Fallback Threshold

`queryParser.ts` runs deterministic-first, extracting intent from obvious patterns (`"pdf"`, `"from 2024"`, `"in Documents"`). LLM fallback handles ambiguous free-form queries.

**Fallback rule:** invoke the LLM only when both:
- Query is **longer than 5 words**
- More than **3 tokens remain unrecognised** after the deterministic pass

Short queries and queries where most tokens were consumed deterministically never hit the LLM. LLM is never called in `mode: 'keyword'`.

**Why explicit thresholds:** deterministic, testable, easy to tune. LLM call adds 200-800ms + requires Ollama — cost justifies a high bar.

JSON parsing of LLM responses uses 3-tier fallback: `JSON.parse` → brace-balanced extraction → regex field salvage.

---

## 20. Privacy UX — Explicit controls

Scope editor + audit log + delete index.

| Setup | Coverage | Trust lift | Support incidents |
|---|---|---|---|
| Baseline | 0% | +0.125 | 50% |
| Explicit controls | 100% | +0.500 | 0% |

**UX constraint:** each control shows current status at a glance (always visible); details and configuration are collapsed by default and only expanded on user interaction.

---

## 21. Duplicate Presentation — Listed separately

Functional requirement. Duplicates appear as separate result entries in v1. No grouped presentation.

---

## 22. Eval Harness

In v1. Validates retrieval-quality decisions before Phase D exits.

**Corpus:** mixed pt-BR + en, 30-50 files including 5-10 images covering pure-photo / screenshot / text-bearing / diagram. Uses real samples in `desktop/test/sample-files/`.

**Queries:** 30-50 hand-curated `{ query, expected_file_paths[] }` mappings. Initial set drafted from corpus inspection by the developer; reviewed and corrected by the user. Includes 3-5 cross-language queries (English query → pt-BR doc, vice versa).

**Metrics:** Recall@K, MRR, NDCG via existing `metrics.ts` (TS, pure).

**Architecture:** native Rust eval binary at `src-tauri/src/bin/eval.rs` runs the curated corpus through the real `search_files()` (no Tauri shell, no IPC). Emits per-query JSON. TS metrics consumer reads JSON, computes scores, writes results.

**Persistence:**
- `test/fixtures/eval-results/<YYYY-MM-DD>-<sha>.json` — per-run results, committed.
- `test/fixtures/eval-results/baseline.json` — current accepted baseline.
- `test/fixtures/eval-results/HISTORY.md` — append-only human-readable progress log.

---

## 23. Test Layering

**CI (every PR):** unit (`npm test`, `cargo test`) + types (`npx tsc --noEmit`) + lint (`cargo clippy`, `cargo fmt --check`). No Ollama. <2 min runtime.

**Local pre-push gate (developer's Mac):** full suite — unit, integration with **live Ollama** (`cargo test --features live-ollama`, `npm run test:integration`), E2E (Phase E), eval, eval-diff, types, lint. Live Ollama is the only honest target environment for this product.

Rationale: GPU-less Linux CI cannot meaningfully run `qwen2.5vl:7b` (CPU is 30-90s/image); model downloads are 5+ GB; tests would be slow and flaky. The developer's Mac IS the target end-user environment; testing there has zero stub-vs-reality drift.

---

## 24. Pre-Push Hook

Versioned shell script at `scripts/git-hooks/pre-push`. Installed via one-line `npm run setup` (creates a symlink). Zero new dependencies — no husky.

The hook runs the full Layer-2 pre-push gate. Bypassable with `--no-verify` for emergencies; not the default path.

---

## 25. Eval Regression Gate

`npm run eval:diff` fails the push if **either** Recall@10 OR MRR drops by **>5% absolute** versus `baseline.json`. Threshold tunable as noise patterns emerge.

---

## 26. Eval Baseline Lifecycle

Baseline never changes implicitly. `npm run eval:promote` explicitly copies the latest run to `baseline.json` after the developer reviews deltas. Promote-on-purpose, regress-on-accident.

`HISTORY.md` records what changed and why ("2026-04-29: prefix fix → Recall@10 0.31 → 0.78"). Append-only; committed alongside the new baseline.

---

## 27. Coverage Target

80% line coverage on `src/core/` and `src-tauri/src/`. Excluded: `lib.rs` (Tauri command glue), `platform/`, `bin/` (eval binary).

Verified by `vitest --coverage` and `cargo tarpaulin` (or equivalent). Gate added to phase exit criteria.

---

## 28. E2E Framework

`tauri-driver` + WebDriverIO. ≤10 happy-path scenarios protecting critical journeys: onboarding, add root, index, search, open file, settings → delete index. Lands in Phase E exit criteria.

Rationale for low scenario count: unit/integration cover behaviour; E2E proves the wiring works. Don't try to E2E-test everything — flake budget is finite.

---

## 29. Quality Gates Live in Phases

No separate "Phase G — Quality Gates". Coverage targets, integration tests, and (in Phase E) E2E are exit-criteria additions to existing phases A-F. Quality work as a standalone phase always slips.

---

## 30. Phase D Rework — Migration-aligned groups

The Phase D retrieval-quality fixes ship in three migration-aligned groups, with eval-diff between each group. This isolates impact attribution and minimises user-facing migrations.

**Group A — No reindex required.**
Snippet rework (decisions 16, 17), file-type expansion (decision 18). Pure code changes. Eval-diff against baseline.

**Group B — Re-embedding only.**
Model-agnostic config (decision 7), prefixes (decision 9), L2 normalize + cosine (decision 8), chunk size 256→512 (decision 4). Bump `model_version` → existing chunks re-embed but extracted text is preserved. No re-extraction, no re-OCR. Eval-diff against group-A baseline.

**Group C — Image re-extraction.**
Vision determinism + unified prompt (decision 15). Existing image rows re-extract via vision pipeline (one new Ollama call per image). Eval-diff against group-B baseline.

Each group lands as a self-contained migration. The eval-diff between groups attributes relevance lift to the right change.

### Group-specific eval-diff gates

The pre-push gate (decision 25, "no >5% drop") is a *don't-regress* check. Each group has additional intent-aware gates run via `npm run eval:diff --group <A|B|C>`:

| Group | Gate | Rationale |
|---|---|---|
| A | Within ±2% on Recall@10 *and* MRR vs. previous baseline | No relevance change is expected — snippets and file-type routing don't touch ranking. Anything outside ±2% is a red flag |
| B | Recall@10 ≥ +5% abs *and* MRR ≥ +5% abs vs. group-A baseline | The prefix fix is the headline win — flat metrics here mean the fix didn't take effect |
| C | Image-subset Recall@10 ≥ +5% abs vs. group-B baseline | Vision changes only touch image queries (5-10 of ~40 files); overall metrics may barely move. Slice by `media_type` |

A group does not exit until its gate passes. Failing the gate triggers the diagnosis tree below.

### Migration safety: feature branches only

The on-disk index is forward-only — once a migration runs, there is no cheap revert. Therefore:

- Each group lands on a **feature branch** with `model_version = "<group>-experimental"`. Migration runs locally; eval runs locally; results commit to the branch.
- **Merge gate:** the group's eval-diff gate must pass on the branch. On merge, `model_version` becomes `"<group>-final"`.
- **Failure path:** discard the branch. The developer's local index gets re-migrated by whichever group lands next on `main`. **There is no user-index rollback code path** — by construction, no failed group ever reaches users.

### Group B diagnosis tree (run if the +5% lift gate fails)

The recommendation that prefixes are the largest single source of irrelevance is an *audit-based prediction*, not a measurement. If Group B's eval shows <5% lift, do not declare Phase D done. Run this diagnosis sequence:

1. **Prefix string variants.** Try `"search_query: "`, `"query: "`, and a no-prefix-but-normalize-only branch on three sub-branches. Eval each. Keep the winner.
2. **Verify embeddings are actually different.** Pick two chunks; embed them with and without prefix; assert the vectors differ meaningfully (cosine similarity to each other should drop notably with prefixes — if it doesn't, the prefix is being stripped/ignored upstream).
3. **Bottleneck reattribution.** If prefix variants are flat, the bottleneck is elsewhere. Try chunk size variants in isolation (256, 512, 800) — keep the prefix fix from step 1; rerun eval.
4. **Corpus / query coverage.** If the eval set is <30 queries, single bad queries can swing the mean. Add queries (cross-language, conceptual, image-content) and re-baseline before declaring failure.
5. **Escalate Option D early.** Swap to `bge-m3` (1024-dim, multilingual) on a branch. If `bge-m3` blows past nomic-v2-moe on the same corpus, **escalate Option D from post-v1 to v1** — change of model is no harder than change of prefix once decision 7 (model-agnostic config) is in place.

Document the chosen path in `HISTORY.md` so the rationale survives the conversation.

---

## 31. Pipeline Versioning — Split extraction / embedding versions

The current `files.model_version` column conflates the extraction pipeline version with the embedding pipeline version. Group B (decision 30) changes embeddings only — re-extracting every PDF/image would waste hours of OCR and vision calls.

**Schema change (one-shot migration):**

```sql
ALTER TABLE files ADD COLUMN extraction_version TEXT NOT NULL DEFAULT '';
ALTER TABLE files ADD COLUMN embedding_version  TEXT NOT NULL DEFAULT '';
-- Backfill: copy current model_version into extraction_version; clear embedding_version.
UPDATE files SET extraction_version = model_version WHERE model_version != '';
ALTER TABLE files DROP COLUMN model_version;
```

**Fields after the migration:**

- `extraction_version`: format `"ext-v<n>"` (e.g. `"ext-v1"`). Bump when extractor logic, OCR confidence threshold, or vision prompt change. Clearing this triggers re-extraction.
- `embedding_version`: format `"<model>:<dim>:<flags>"` (e.g. `"nomic-embed-text-v2-moe:768:prefixed-l2"`). Bump when model, dimension, prefix convention, or normalisation change. Clearing this triggers re-embedding only.

**Indexer logic:**

| Empty / mismatched | Effect |
|---|---|
| `extraction_version` | Re-extract → re-chunk → re-embed (full pipeline) |
| `embedding_version` only | Re-chunk (using stored `extracted_text`) → re-embed; skip extraction |
| Both current | Skip file entirely |

The extracted text is preserved in `files.extracted_text` regardless, so re-embedding never hits Ollama for vision/OCR.

## 32. Re-embed UX — Silent, existing progress UI

When Group B merges and a user opens the app, the indexer detects embedding_version mismatch and re-embeds existing chunks. No opt-in, no separate banner — Group B is unconditionally a search-quality improvement.

The existing `indexing://progress` event covers this; the indexer emits a new `phase: "embedding"` value (alongside `discovering`, `fingerprinting`, `extracting`). The frontend `IndexingProgress.tsx` shows the phase label — copy reads "Improving search quality" when the active phase is `embedding` on a previously-indexed root.

Re-embedding is fast (no OCR or vision); typical personal corpus completes in minutes.

---

## 33. Returning-User UX — Versioned "What's New" sheet + re-embed toast

Onboarding remains one-shot — `wtf:onboarded = true` is never cleared once set. Re-onboarding feels like a regression to users who already configured the app.

**`wtf:lastSeenVersion`** (localStorage). On first launch after upgrade, if `lastSeenVersion < currentAppVersion`, show a non-blocking "What's New" sheet listing user-visible changes for that release. Dismissed by user → `lastSeenVersion = currentAppVersion`. The sheet has no consent affordance; it is informational.

**Re-embed toast (Group B specifically).** When the indexer detects Group B's `embedding_version` mismatch on launch, show a non-blocking, auto-dismissing toast: *"Updated search quality — re-indexing existing files. This takes a few minutes and runs locally."* Independent of the "What's New" sheet — the toast tells the user *why their app is busy*, the sheet describes *what's new in this release*.

**"Skip for now" users.** No new UX. Their existing `SkipWarningBanner` remains the only nudge. Group B is a no-op for them (no chunks to re-embed); they still need to configure a root before search works.

---

## 34. Live-Ollama Test Gating — `#[cfg_attr(..., ignore)]`

Live-Ollama tests are gated by an attribute that flips between *ignored* and *run*, but the test body is **always compiled**. CI catches API drift and typos even though it cannot run the tests.

```rust
#[test]
#[cfg_attr(not(feature = "live-ollama"), ignore = "needs Ollama")]
fn search_with_real_ollama_returns_results() {
    let url = std::env::var("OLLAMA_TEST_URL")
        .unwrap_or_else(|_| "http://localhost:11434".into());
    // ...
}
```

- **Default `cargo test`:** compiles every test; skips live ones with `ignored` output. CI runs this.
- **`cargo test --features live-ollama`:** compiles and runs everything. Local pre-push runs this.
- **`OLLAMA_TEST_URL` env var** overrides the default `localhost:11434` for developers running Ollama on non-default ports (Docker on Windows, alternate hosts).

The Cargo feature is declared in `desktop/src-tauri/Cargo.toml` as `live-ollama = []` — empty deps; it only flips the attribute.

Reject `#[cfg(feature = "live-ollama")]` on whole test functions: that pattern stops CI from compiling the test body and lets type errors ride to local runs.

---

## 35. Activity Log — Coarse migration events

Re-embedding a chunk is an internal data transformation, not new processing of user file content. Per-file activity-log entries for migrations would drown out the user-relevant entries.

**One coarse `pipeline_migrated` event per Group B/C migration**, written when the migration completes:

```json
{
  "event_type": "pipeline_migrated",
  "detail": {
    "group": "B",
    "files_affected": 1247,
    "chunks_affected": 18603,
    "duration_seconds": 312,
    "from": { "embedding_version": "nomic-embed-text-v2-moe:768:legacy" },
    "to":   { "embedding_version": "nomic-embed-text-v2-moe:768:prefixed-l2" }
  }
}
```

The activity-log UI renders the summary line ("Pipeline upgraded: 1,247 files re-embedded for improved search quality"); the full JSON detail is available on click. Aligns with how OS-level audit logs surface system updates: one entry per release, not one per file.

This decision honours the privacy-trust principle (user can audit all processing) without polluting the log on every migration.

---

## 36. macOS 26 Startup Stability — Tao/Wry Patch Set

On macOS 26 (Tahoe), startup crashed during `applicationDidFinishLaunching` with `panic in a function that cannot unwind` / `fatal runtime error: Rust cannot catch foreign exceptions, aborting`. Crash site: Tao app delegate launch callback.

**Locked workaround for current pinned versions (`tao 0.34.8`, `wry 0.54.4`):**
- `app.macOSPrivateApi = true` in Tauri config when using transparent windows.
- Tauri Rust feature `macos-private-api` enabled.
- Patched Tao uses `extern "C-unwind"` for `applicationDidFinishLaunching` callback.
- objc2 `relax-sign-encoding` enabled for patched Tao and Wry Apple targets.

Changes are constrained to local patched crates under `desktop/src-tauri/patches/` and Tauri app config.

**Removal condition:** drop local patches after upgrading to upstream Tao/Wry versions that boot cleanly on macOS 26 without these modifications.
