# ADR 0001 — Query parser orchestration lives in TypeScript

Date: 2026-06-01
Status: Accepted

## Context

The natural-language query parser turns a user string into a structured `ParsedQuery`. The work is split between two languages:

- A **deterministic pass** in TypeScript ([`desktop/src/core/queryParser.ts`](../../desktop/src/core/queryParser.ts)) that extracts media types, dates, scope, and confidence using lexical rules.
- An **LLM fallback** in Rust ([`desktop/src-tauri/src/llm/query_parser.rs`](../../desktop/src-tauri/src/llm/query_parser.rs)) that asks Ollama to re-parse queries the deterministic pass couldn't fully resolve.

Before this refactor, the *strategy* — when to run the LLM, what to do when it fails — lived in the React hook `useSearch`. Issue #15 made the seam one function, `parseQuery(input, mode): Promise<ParsedQuery>`. Where that orchestrator should live was the open question.

## Decision

Orchestration lives in TypeScript. The TS `parseQuery` runs the deterministic pass synchronously and, when its internal gate triggers, calls into Rust for the LLM pass via dependency injection. Rust exposes only the LLM pass.

The LLM-fallback dependency is **injected**, not imported, so `core/queryParser.ts` stays Tauri-free and unit-testable without a Tauri shell. A small composition root ([`desktop/src/hooks/parseQueryDefault.ts`](../../desktop/src/hooks/parseQueryDefault.ts)) wires the production `parseQueryLlm` into `parseQuery` so every caller stays one expression.

## Alternatives considered

**Orchestration in Rust.** Both passes live in `llm/query_parser.rs`; TS becomes a thin IPC wrapper. Rejected because every keystroke would pay an IPC + LLM round-trip even when the deterministic pass would have answered locally. The deterministic pass running synchronously in TS is a measurable UX win we did not want to give up.

## Consequences

- The deterministic fast path stays synchronous and free of IPC cost.
- The LLM round-trip is only incurred when the gate triggers (>5 words, >3 unresolved tokens, non-keyword mode).
- The seam (`parseQuery`) returns a single `Promise<ParsedQuery>` — callers no longer know the strategy is two-phase. The hook stops branching on `needsLlmFallback`.
- `useSearch` no longer renders results twice on fallback-eligible queries; instead it renders once after the LLM (or deterministic fallback) completes. The previous "render fast, refine later" UX is gone for those queries. Most queries do not hit the gate, so most queries are unaffected.
- LLM failures are swallowed inside `parseQuery` (a `console.warn` is emitted) and the deterministic result is returned. Graceful degradation matches the pre-refactor behavior; the caller does not need to handle this case.
- The `MEDIA_TYPES` list now has a single source of truth in [`desktop/src/core/mediaTypes.json`](../../desktop/src/core/mediaTypes.json). TS imports it directly; Rust generates a `const` from it at build time via `build.rs`. Two parity tests guard the contract.
