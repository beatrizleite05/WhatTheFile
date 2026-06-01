# CONTEXT — WhatTheFile domain glossary

Single-source domain language for the project. Terms here are load-bearing — they appear in code, docs, and ADRs and should be used consistently.

## Query Parser

The component that turns a user's natural-language input string into a structured `ParsedQuery` (media types, dates, root scope, confidence threshold, free-text). Public seam: `parseQuery(input, mode, { llmFallback }): Promise<ParsedQuery>` in [`desktop/src/core/queryParser.ts`](desktop/src/core/queryParser.ts).

Internally two-phase: a synchronous deterministic pass extracts known patterns; an LLM fallback (injected, implemented in Rust at [`desktop/src-tauri/src/llm/query_parser.rs`](desktop/src-tauri/src/llm/query_parser.rs)) re-parses when the deterministic pass leaves the query under-resolved. The decision to call the LLM lives inside the parser, not at the call site. See [ADR 0001](docs/adr/0001-query-parser-orchestration-in-typescript.md).
