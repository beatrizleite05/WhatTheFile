# Phase A — Project Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scaffold the full `desktop/` Tauri project structure and implement all 10 `src/core/` TypeScript modules so every existing test passes.

**Architecture:** Migrate existing root-level test/config files into `desktop/`, implement pure-logic TypeScript modules inside `desktop/src/core/` following TDD (tests already exist), add a minimal React + Vite frontend skeleton, write compilable Rust stubs with `todo!()` bodies, and wire a CI workflow.

**Tech Stack:** Tauri v2, React 18, TypeScript 5 (strict), Vite 5, Vitest 3, Node.js `crypto` (built-in), Rust + thiserror + tokio, GitHub Actions

> **Git note:** All `git` commands in this plan must be run from the **repo root** (`WhatTheFile/`), not from inside `desktop/`. File paths in `git add` are relative to the repo root.

---

## File Map

| Action | Path | Responsibility |
|---|---|---|
| Move | `package.json` → `desktop/package.json` | npm scripts + all deps |
| Move | `tsconfig.json` → `desktop/tsconfig.json` | TypeScript config (add JSX + DOM) |
| Move | `vitest.config.ts` → `desktop/vitest.config.ts` | Test glob (unchanged) |
| Move | `test/` → `desktop/test/` | All 11 test files (imports unchanged) |
| Create | `desktop/vite.config.ts` | Vite + React plugin |
| Create | `desktop/index.html` | Vite HTML entry |
| Create | `desktop/public/.gitkeep` | Static asset dir |
| Create | `desktop/src/core/types.ts` | All shared TypeScript interfaces |
| Create | `desktop/src/core/pagination.ts` | `normalizePagination` |
| Create | `desktop/src/core/fingerprint.ts` | `buildFingerprint` (Node crypto) |
| Create | `desktop/src/core/indexPlanner.ts` | `planIndexAction` |
| Create | `desktop/src/core/chunker.ts` | `chunkText` |
| Create | `desktop/src/core/eventCoalescer.ts` | `coalesceEvents` |
| Create | `desktop/src/core/ranking.ts` | `rankFiles` |
| Create | `desktop/src/core/policy.ts` | `normalizeExt`, `shouldIndexFile` |
| Create | `desktop/src/core/metrics.ts` | `recallAtK`, `mrr`, `ndcgAtK` |
| Create | `desktop/src/core/queryParser.ts` | `parseNaturalLanguageQuery` |
| Create | `desktop/src/App.tsx` | Minimal placeholder |
| Create | `desktop/src/main.tsx` | React entry point |
| Create | `desktop/src/api/search.ts` | Tauri invoke stub |
| Create | `desktop/src/api/indexing.ts` | Tauri invoke stub |
| Create | `desktop/src/api/settings.ts` | Tauri invoke stub |
| Create | `desktop/src/api/runtime.ts` | Tauri invoke stub |
| Create | `desktop/src/api/index.ts` | Re-exports |
| Create | `desktop/src/hooks/useSearch.ts` | Phase E stub |
| Create | `desktop/src/hooks/useIndexing.ts` | Phase E stub |
| Create | `desktop/src/hooks/useSettings.ts` | Phase E stub |
| Create | `desktop/src/hooks/useOllamaStatus.ts` | Phase E stub |
| Create | `desktop/src/utils/.gitkeep` | Placeholder |
| Create | `desktop/src/components/.gitkeep` | Placeholder |
| Create | `desktop/src-tauri/Cargo.toml` | Rust deps |
| Create | `desktop/src-tauri/build.rs` | Tauri build script |
| Create | `desktop/src-tauri/tauri.conf.json` | App config |
| Create | `desktop/src-tauri/capabilities/.gitkeep` | Tauri v2 capabilities |
| Create | `desktop/src-tauri/icons/.gitkeep` | App icons |
| Create | `desktop/src-tauri/migrations/.gitkeep` | SQL migration files |
| Create | `desktop/src-tauri/src/main.rs` | Binary entry point |
| Create | `desktop/src-tauri/src/lib.rs` | Tauri command layer |
| Create | `desktop/src-tauri/src/errors.rs` | Central `AppError` enum |
| Create | `desktop/src-tauri/src/db.rs` | SQLite stub |
| Create | `desktop/src-tauri/src/indexer.rs` | Indexer stub |
| Create | `desktop/src-tauri/src/extractor.rs` | Extractor stub |
| Create | `desktop/src-tauri/src/chunker.rs` | Chunker stub |
| Create | `desktop/src-tauri/src/search.rs` | Search stub |
| Create | `desktop/src-tauri/src/config.rs` | Config stub |
| Create | `desktop/src-tauri/src/llm/mod.rs` | LLM module root |
| Create | `desktop/src-tauri/src/llm/embeddings.rs` | Embedding stub |
| Create | `desktop/src-tauri/src/llm/vision.rs` | Vision stub |
| Create | `desktop/src-tauri/src/llm/runtime.rs` | Ollama lifecycle stub |
| Create | `desktop/src-tauri/src/platform/mod.rs` | OS-specific stubs |
| Create | `.github/workflows/ci.yml` | Typecheck + test on push |
| Create | `shared/.gitkeep` | Reserved for cross-layer schemas |
| Create | `scripts/.gitkeep` | Build/release scripts |

---

## Task 1: Scaffold `desktop/` and migrate existing files

**Files:**
- Create: `desktop/` directory tree
- Move: `package.json` → `desktop/package.json`
- Move: `tsconfig.json` → `desktop/tsconfig.json`
- Move: `vitest.config.ts` → `desktop/vitest.config.ts`
- Move: `test/` → `desktop/test/`

- [ ] **Step 1: Create directory skeleton**

```bash
mkdir -p desktop/src/core
mkdir -p desktop/src/hooks
mkdir -p desktop/src/components
mkdir -p desktop/src/utils
mkdir -p desktop/src/api
mkdir -p desktop/test
mkdir -p desktop/public
mkdir -p desktop/src-tauri/src/llm
mkdir -p desktop/src-tauri/src/platform
mkdir -p desktop/src-tauri/capabilities
mkdir -p desktop/src-tauri/icons
mkdir -p desktop/src-tauri/migrations
mkdir -p .github/workflows
mkdir -p shared
mkdir -p scripts
```

- [ ] **Step 2: Move root files into desktop/**

```bash
mv package.json desktop/package.json
mv tsconfig.json desktop/tsconfig.json
mv vitest.config.ts desktop/vitest.config.ts
mv test desktop/test
```

If `src/` exists at the root (e.g. a partial `src/core/fingerprint.ts`), also run:
```bash
# Only if src/ exists at root:
mv src desktop/src
```

- [ ] **Step 3: Write `desktop/package.json`**

Replace its contents entirely:

```json
{
  "name": "whatthefile",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "test": "vitest run",
    "test:watch": "vitest",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "react": "^18",
    "react-dom": "^18"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@types/node": "^25.5.0",
    "@types/react": "^18",
    "@types/react-dom": "^18",
    "@vitejs/plugin-react": "^4",
    "typescript": "^5.9.2",
    "vite": "^5",
    "vitest": "^3.2.4"
  }
}
```

- [ ] **Step 4: Write `desktop/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "jsx": "react-jsx",
    "lib": ["ES2022", "DOM"],
    "types": ["vitest/globals"],
    "noEmit": true,
    "skipLibCheck": true
  },
  "include": ["src", "test", "vitest.config.ts", "vite.config.ts"]
}
```

- [ ] **Step 5: Install dependencies**

```bash
cd desktop && npm install
```

Expected: `node_modules/` created, no errors.

- [ ] **Step 6: Confirm tests fail (expected — no implementations yet)**

```bash
cd desktop && npm test 2>&1 | head -20
```

Expected: errors like `Cannot find module '../src/core/pagination'`. This is correct — implementations come next.

- [ ] **Step 7: Commit**

```bash
git add desktop/ shared/ scripts/
git commit -m "chore: scaffold desktop/ structure and migrate root files"
```

---

## Task 2: Vite + React frontend skeleton

**Files:**
- Create: `desktop/vite.config.ts`
- Create: `desktop/index.html`
- Create: `desktop/src/main.tsx`
- Create: `desktop/src/App.tsx`
- Create: `desktop/public/.gitkeep`

- [ ] **Step 1: Write `desktop/vite.config.ts`**

```typescript
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: ['es2021', 'chrome100', 'safari13'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
```

- [ ] **Step 2: Write `desktop/index.html`**

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>WhatTheFile</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 3: Write `desktop/src/main.tsx`**

```typescript
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

- [ ] **Step 4: Write `desktop/src/App.tsx`**

```typescript
export default function App() {
  return <div>WhatTheFile</div>;
}
```

- [ ] **Step 5: Write `desktop/public/.gitkeep`**

```
```

(empty file)

- [ ] **Step 6: Commit**

```bash
cd desktop && git add . && git commit -m "chore: add Vite + React frontend skeleton"
```

---

## Task 3: Create `types.ts` (foundation for all modules)

**Files:**
- Create: `desktop/src/core/types.ts`

No test file exists for `types.ts` directly — it is imported by all other modules.

- [ ] **Step 1: Write `desktop/src/core/types.ts`**

```typescript
export interface FileResult {
  fileId: number;
  rootId: number;
  path: string;
  filename: string;
  mediaType: string;
  sizeBytes: number;
  indexedAt: number;
  confidence: number;
  snippet: string;
  score: number;
}

export interface Chunk {
  startToken: number;
  endToken: number;
  text: string;
}

export interface ScanPolicy {
  includeRoots: string[];
  excludeGlobs: string[];
  excludedExtensions: string[];
  maxFileSizeBytes: number;
  includeHidden: boolean;
}

export interface SearchRequest {
  queryText: string;
  mediaTypes: string[];
  rootScope: string[];
  dateFrom: string;
  dateTo: string;
  minConfidence: number;
  parserConfidence: number;
}

export interface FileState {
  path: string;
  hash: string;
  mtimeMs: number;
  sizeBytes: number;
}

export interface FsEvent {
  path: string;
  type: 'created' | 'modified' | 'deleted';
  ts: number;
}
```

- [ ] **Step 2: Commit**

```bash
cd desktop && git add src/core/types.ts && git commit -m "feat: add core TypeScript types"
```

---

## Task 4: Implement `pagination.ts`

**Files:**
- Create: `desktop/src/core/pagination.ts`
- Test: `desktop/test/pagination.test.ts`

- [ ] **Step 1: Confirm the test fails**

```bash
cd desktop && npx vitest run test/pagination.test.ts 2>&1 | tail -5
```

Expected: `Cannot find module '../src/core/pagination'`

- [ ] **Step 2: Write `desktop/src/core/pagination.ts`**

```typescript
interface PaginationInput {
  limit: number;
  offset: number;
  maxLimit: number;
}

export interface PaginationResult {
  limit: number;
  offset: number;
}

export function normalizePagination({ limit, offset, maxLimit }: PaginationInput): PaginationResult {
  return {
    limit: Math.min(Math.max(limit, 1), maxLimit),
    offset: Math.max(offset, 0),
  };
}
```

- [ ] **Step 3: Run the test**

```bash
cd desktop && npx vitest run test/pagination.test.ts
```

Expected: `2 tests passed`

- [ ] **Step 4: Commit**

```bash
cd desktop && git add src/core/pagination.ts && git commit -m "feat: implement pagination normalization"
```

---

## Task 5: Implement `fingerprint.ts`

**Files:**
- Create: `desktop/src/core/fingerprint.ts`
- Test: `desktop/test/fingerprint.test.ts`

- [ ] **Step 1: Confirm the test fails**

```bash
cd desktop && npx vitest run test/fingerprint.test.ts 2>&1 | tail -5
```

Expected: `Cannot find module '../src/core/fingerprint'`

- [ ] **Step 2: Write `desktop/src/core/fingerprint.ts`**

```typescript
import { createHash } from 'node:crypto';

export function buildFingerprint(content: string, mtimeMs: number, sizeBytes: number): string {
  return createHash('sha256')
    .update(`${content}:${mtimeMs}:${sizeBytes}`)
    .digest('hex');
}
```

- [ ] **Step 3: Run the test**

```bash
cd desktop && npx vitest run test/fingerprint.test.ts
```

Expected: `2 tests passed`

- [ ] **Step 4: Commit**

```bash
cd desktop && git add src/core/fingerprint.ts && git commit -m "feat: implement fingerprint via SHA-256"
```

---

## Task 6: Implement `indexPlanner.ts`

**Files:**
- Create: `desktop/src/core/indexPlanner.ts`
- Test: `desktop/test/indexPlanner.test.ts`

- [ ] **Step 1: Confirm the test fails**

```bash
cd desktop && npx vitest run test/indexPlanner.test.ts 2>&1 | tail -5
```

Expected: `Cannot find module '../src/core/indexPlanner'`

- [ ] **Step 2: Write `desktop/src/core/indexPlanner.ts`**

```typescript
import type { FileState } from './types';

export type IndexAction = 'index' | 'delete' | 'noop' | 'reindex';

export function planIndexAction(
  existing: FileState | null,
  current: FileState | null
): IndexAction {
  if (existing === null) return 'index';
  if (current === null) return 'delete';
  if (existing.hash === current.hash) return 'noop';
  return 'reindex';
}
```

- [ ] **Step 3: Run the test**

```bash
cd desktop && npx vitest run test/indexPlanner.test.ts
```

Expected: `4 tests passed`

- [ ] **Step 4: Commit**

```bash
cd desktop && git add src/core/indexPlanner.ts && git commit -m "feat: implement index action planner"
```

---

## Task 7: Implement `chunker.ts`

**Files:**
- Create: `desktop/src/core/chunker.ts`
- Test: `desktop/test/chunker.test.ts`

- [ ] **Step 1: Confirm the test fails**

```bash
cd desktop && npx vitest run test/chunker.test.ts 2>&1 | tail -5
```

Expected: `Cannot find module '../src/core/chunker'`

- [ ] **Step 2: Write `desktop/src/core/chunker.ts`**

```typescript
import type { Chunk } from './types';

export interface ChunkOptions {
  chunkSizeTokens: number;
  overlapTokens: number;
}

export function chunkText(text: string, options: ChunkOptions): Chunk[] {
  const { chunkSizeTokens, overlapTokens } = options;

  if (overlapTokens >= chunkSizeTokens) {
    throw new Error(
      `overlapTokens (${overlapTokens}) must be less than chunkSizeTokens (${chunkSizeTokens})`
    );
  }

  const tokens = text.split(/\s+/).filter(t => t.length > 0);
  if (tokens.length === 0) return [];

  const chunks: Chunk[] = [];
  const step = chunkSizeTokens - overlapTokens;

  for (let start = 0; start < tokens.length; start += step) {
    const end = Math.min(start + chunkSizeTokens, tokens.length);
    chunks.push({
      startToken: start,
      endToken: end,
      text: tokens.slice(start, end).join(' '),
    });
    if (end === tokens.length) break;
  }

  return chunks;
}
```

- [ ] **Step 3: Run the test**

```bash
cd desktop && npx vitest run test/chunker.test.ts
```

Expected: `3 tests passed`

- [ ] **Step 4: Commit**

```bash
cd desktop && git add src/core/chunker.ts && git commit -m "feat: implement text chunker with sliding window"
```

---

## Task 8: Implement `eventCoalescer.ts`

**Files:**
- Create: `desktop/src/core/eventCoalescer.ts`
- Test: `desktop/test/eventCoalescer.test.ts`

- [ ] **Step 1: Confirm the test fails**

```bash
cd desktop && npx vitest run test/eventCoalescer.test.ts 2>&1 | tail -5
```

Expected: `Cannot find module '../src/core/eventCoalescer'`

- [ ] **Step 2: Write `desktop/src/core/eventCoalescer.ts`**

```typescript
import type { FsEvent } from './types';

export function coalesceEvents(events: FsEvent[]): FsEvent[] {
  // Map preserves insertion order of first-seen keys
  const latest = new Map<string, FsEvent>();
  for (const event of events) {
    latest.set(event.path, event);
  }
  return Array.from(latest.values());
}
```

- [ ] **Step 3: Run the test**

```bash
cd desktop && npx vitest run test/eventCoalescer.test.ts
```

Expected: `2 tests passed`

- [ ] **Step 4: Commit**

```bash
cd desktop && git add src/core/eventCoalescer.ts && git commit -m "feat: implement filesystem event coalescer"
```

---

## Task 9: Implement `ranking.ts`

**Files:**
- Create: `desktop/src/core/ranking.ts`
- Test: `desktop/test/ranking.test.ts`

- [ ] **Step 1: Confirm the test fails**

```bash
cd desktop && npx vitest run test/ranking.test.ts 2>&1 | tail -5
```

Expected: `Cannot find module '../src/core/ranking'`

- [ ] **Step 2: Write `desktop/src/core/ranking.ts`**

```typescript
import type { FileResult } from './types';

export function rankFiles(results: FileResult[], topK: number): FileResult[] {
  if (topK <= 0) return [];
  return [...results]
    .sort((a, b) => {
      if (b.score !== a.score) return b.score - a.score;
      return a.path.localeCompare(b.path);
    })
    .slice(0, topK);
}
```

- [ ] **Step 3: Run the test**

```bash
cd desktop && npx vitest run test/ranking.test.ts
```

Expected: `6 tests passed`

- [ ] **Step 4: Commit**

```bash
cd desktop && git add src/core/ranking.ts && git commit -m "feat: implement display-layer file ranking"
```

---

## Task 10: Implement `policy.ts`

**Files:**
- Create: `desktop/src/core/policy.ts`
- Test: `desktop/test/policy.test.ts`, `desktop/test/policy.filetypes.test.ts`

- [ ] **Step 1: Confirm the tests fail**

```bash
cd desktop && npx vitest run test/policy.test.ts test/policy.filetypes.test.ts 2>&1 | tail -5
```

Expected: `Cannot find module '../src/core/policy'`

- [ ] **Step 2: Write `desktop/src/core/policy.ts`**

```typescript
import { extname, basename } from 'node:path';
import type { ScanPolicy } from './types';

export function normalizeExt(filePath: string): string {
  const ext = extname(filePath);
  return ext.startsWith('.') ? ext.slice(1).toLowerCase() : ext.toLowerCase();
}

function globToRegex(glob: string): RegExp {
  const escaped = glob
    .replace(/[.+^${}()|[\]\\]/g, '\\$&')
    .replace(/\*\*/g, '\u0000GLOBSTAR\u0000')
    .replace(/\*/g, '[^/]*')
    .replace(/\u0000GLOBSTAR\u0000/g, '.*');
  return new RegExp(`^${escaped}$`);
}

export function shouldIndexFile(
  filePath: string,
  sizeBytes: number,
  policy: ScanPolicy
): boolean {
  const normalized = filePath.replace(/\\/g, '/');

  // Must be under an included root
  const inRoot = policy.includeRoots.some(root =>
    normalized.startsWith(root.replace(/\\/g, '/'))
  );
  if (!inRoot) return false;

  // Reject hidden files (filename starts with '.')
  if (!policy.includeHidden && basename(filePath).startsWith('.')) return false;

  // Reject excluded extensions
  if (policy.excludedExtensions.includes(normalizeExt(filePath))) return false;

  // Reject oversized files
  if (sizeBytes > policy.maxFileSizeBytes) return false;

  // Reject files matching any exclude glob
  if (policy.excludeGlobs.some(glob => globToRegex(glob).test(normalized))) return false;

  return true;
}
```

- [ ] **Step 3: Run the tests**

```bash
cd desktop && npx vitest run test/policy.test.ts test/policy.filetypes.test.ts
```

Expected: `6 tests passed`

- [ ] **Step 4: Commit**

```bash
cd desktop && git add src/core/policy.ts && git commit -m "feat: implement file indexing policy (allow/deny)"
```

---

## Task 11: Implement `metrics.ts`

**Files:**
- Create: `desktop/src/core/metrics.ts`
- Test: `desktop/test/metrics.test.ts`

- [ ] **Step 1: Confirm the test fails**

```bash
cd desktop && npx vitest run test/metrics.test.ts 2>&1 | tail -5
```

Expected: `Cannot find module '../src/core/metrics'`

- [ ] **Step 2: Write `desktop/src/core/metrics.ts`**

```typescript
export function recallAtK(relevant: string[], retrieved: string[], k: number): number {
  if (relevant.length === 0 || k === 0) return 0;
  const relevantSet = new Set(relevant);
  const hits = retrieved.slice(0, k).filter(r => relevantSet.has(r)).length;
  return hits / relevant.length;
}

export function mrr(relevant: string[], retrieved: string[]): number {
  if (relevant.length === 0) return 0;
  const relevantSet = new Set(relevant);
  for (let i = 0; i < retrieved.length; i++) {
    if (relevantSet.has(retrieved[i])) return 1 / (i + 1);
  }
  return 0;
}

export function ndcgAtK(
  graded: Record<string, number>,
  retrieved: string[],
  k: number
): number {
  if (k === 0) return 0;

  const dcg = (items: string[], limit: number): number => {
    let score = 0;
    for (let i = 0; i < Math.min(items.length, limit); i++) {
      score += (graded[items[i]] ?? 0) / Math.log2(i + 2);
    }
    return score;
  };

  const idealDcg = dcg(
    Object.keys(graded).sort((a, b) => (graded[b] ?? 0) - (graded[a] ?? 0)),
    k
  );
  if (idealDcg === 0) return 0;
  return dcg(retrieved, k) / idealDcg;
}
```

- [ ] **Step 3: Run the test**

```bash
cd desktop && npx vitest run test/metrics.test.ts
```

Expected: `9 tests passed`

- [ ] **Step 4: Commit**

```bash
cd desktop && git add src/core/metrics.ts && git commit -m "feat: implement IR evaluation metrics (Recall@K, MRR, NDCG)"
```

---

## Task 12: Implement `queryParser.ts`

**Files:**
- Create: `desktop/src/core/queryParser.ts`
- Test: `desktop/test/queryParser.test.ts`

- [ ] **Step 1: Confirm the test fails**

```bash
cd desktop && npx vitest run test/queryParser.test.ts 2>&1 | tail -5
```

Expected: `Cannot find module '../src/core/queryParser'`

- [ ] **Step 2: Write `desktop/src/core/queryParser.ts`**

```typescript
import type { SearchRequest } from './types';

const MEDIA_TYPES = ['pdf', 'docx', 'xlsx', 'csv', 'txt', 'md', 'png', 'jpg', 'jpeg'];

export function parseNaturalLanguageQuery(input: string): SearchRequest {
  const tokens = input.trim().split(/\s+/).filter(t => t.length > 0);
  const wordCount = tokens.length;
  const consumed = new Set<number>();

  const mediaTypes: string[] = [];
  const rootScope: string[] = [];
  let dateFrom = '';
  let dateTo = '';
  let minConfidence = 0;

  // Extract media types
  for (let i = 0; i < tokens.length; i++) {
    if (MEDIA_TYPES.includes(tokens[i].toLowerCase())) {
      mediaTypes.push(tokens[i].toLowerCase());
      consumed.add(i);
    }
  }

  // Extract "from YYYY"
  for (let i = 0; i < tokens.length - 1 && !dateFrom; i++) {
    if (!consumed.has(i) && tokens[i].toLowerCase() === 'from' && /^\d{4}$/.test(tokens[i + 1])) {
      dateFrom = `${tokens[i + 1]}-01-01`;
      dateTo = `${tokens[i + 1]}-12-31`;
      consumed.add(i);
      consumed.add(i + 1);
    }
  }

  // Extract standalone YYYY
  for (let i = 0; i < tokens.length && !dateFrom; i++) {
    if (!consumed.has(i) && /^\d{4}$/.test(tokens[i])) {
      dateFrom = `${tokens[i]}-01-01`;
      dateTo = `${tokens[i]}-12-31`;
      consumed.add(i);
    }
  }

  // Extract "today"
  for (let i = 0; i < tokens.length && !dateFrom; i++) {
    if (!consumed.has(i) && tokens[i].toLowerCase() === 'today') {
      const today = new Date().toISOString().split('T')[0];
      dateFrom = today;
      dateTo = today;
      consumed.add(i);
    }
  }

  // Extract "in [Scope]" — scope must not be a media type keyword
  for (let i = 0; i < tokens.length - 1; i++) {
    if (!consumed.has(i) && tokens[i].toLowerCase() === 'in' && !consumed.has(i + 1)) {
      const scope = tokens[i + 1].toLowerCase();
      if (!MEDIA_TYPES.includes(scope)) {
        rootScope.push(scope);
        consumed.add(i);
        consumed.add(i + 1);
      }
    }
  }

  // Extract "min confidence N"
  for (let i = 0; i < tokens.length - 2; i++) {
    if (
      !consumed.has(i) &&
      tokens[i].toLowerCase() === 'min' &&
      tokens[i + 1]?.toLowerCase() === 'confidence' &&
      !consumed.has(i + 1) &&
      !consumed.has(i + 2)
    ) {
      const val = parseFloat(tokens[i + 2]);
      if (!isNaN(val)) {
        minConfidence = val;
        consumed.add(i);
        consumed.add(i + 1);
        consumed.add(i + 2);
      }
    }
  }

  const unresolvedTokens = tokens.filter((_, i) => !consumed.has(i));
  const queryText = unresolvedTokens.join(' ') || input.trim();
  const unresolvedCount = unresolvedTokens.length;

  let parserConfidence: number;
  if (wordCount <= 5) {
    parserConfidence = 0.9;
  } else if (unresolvedCount > 3) {
    parserConfidence = 0.2;
  } else {
    parserConfidence = 0.8;
  }

  return { queryText, mediaTypes, rootScope, dateFrom, dateTo, minConfidence, parserConfidence };
}
```

- [ ] **Step 3: Run the test**

```bash
cd desktop && npx vitest run test/queryParser.test.ts
```

Expected: `7 tests passed`

- [ ] **Step 4: Commit**

```bash
cd desktop && git add src/core/queryParser.ts && git commit -m "feat: implement deterministic NL query parser"
```

---

## Task 13: Verify all TypeScript tests pass + type check

- [ ] **Step 1: Run the full test suite**

```bash
cd desktop && npm test
```

Expected output:
```
Test Files  11 passed (11)
Tests       XX passed (XX)
```
All 11 test files must be green. If any fail, fix the implementation before proceeding.

- [ ] **Step 2: Run TypeScript strict check**

```bash
cd desktop && npx tsc --noEmit
```

Expected: no output (zero errors). If errors appear, fix them — no `any`, no missing types.

- [ ] **Step 3: Commit**

```bash
cd desktop && git add . && git commit -m "chore: all 11 core module test suites passing, zero TS errors"
```

---

## Task 14: API stubs (`desktop/src/api/`)

**Files:**
- Create: `desktop/src/api/search.ts`
- Create: `desktop/src/api/indexing.ts`
- Create: `desktop/src/api/settings.ts`
- Create: `desktop/src/api/runtime.ts`
- Create: `desktop/src/api/index.ts`

- [ ] **Step 1: Write `desktop/src/api/search.ts`**

```typescript
import { invoke } from '@tauri-apps/api/core';

export async function search(query: string): Promise<unknown> {
  return invoke('search', { query });
}
```

- [ ] **Step 2: Write `desktop/src/api/indexing.ts`**

```typescript
import { invoke } from '@tauri-apps/api/core';

export async function startIndexing(rootPath: string): Promise<void> {
  return invoke('start_indexing', { rootPath });
}

export async function addRoot(path: string): Promise<void> {
  return invoke('add_root', { path });
}
```

- [ ] **Step 3: Write `desktop/src/api/settings.ts`**

```typescript
// Phase E: settings IPC wrappers
export {};
```

- [ ] **Step 4: Write `desktop/src/api/runtime.ts`**

```typescript
import { invoke } from '@tauri-apps/api/core';

export async function getRuntimeStatus(): Promise<unknown> {
  return invoke('get_runtime_status');
}

export async function openFile(path: string): Promise<void> {
  return invoke('open_file', { path });
}
```

- [ ] **Step 5: Write `desktop/src/api/index.ts`**

```typescript
export * from './search';
export * from './indexing';
export * from './settings';
export * from './runtime';
```

- [ ] **Step 6: Run typecheck to confirm no errors**

```bash
cd desktop && npx tsc --noEmit
```

Expected: zero errors.

- [ ] **Step 7: Commit**

```bash
cd desktop && git add src/api/ && git commit -m "feat: add typed Tauri IPC stubs (api/)"
```

---

## Task 15: Hook + component stubs

**Files:**
- Create: `desktop/src/hooks/useSearch.ts`
- Create: `desktop/src/hooks/useIndexing.ts`
- Create: `desktop/src/hooks/useSettings.ts`
- Create: `desktop/src/hooks/useOllamaStatus.ts`
- Create: `desktop/src/utils/.gitkeep`
- Create: `desktop/src/components/.gitkeep`

- [ ] **Step 1: Write `desktop/src/hooks/useSearch.ts`**

```typescript
// Phase E: search state hook
export function useSearch() {
  // TODO Phase E
}
```

- [ ] **Step 2: Write `desktop/src/hooks/useIndexing.ts`**

```typescript
// Phase E: indexing state hook
export function useIndexing() {
  // TODO Phase E
}
```

- [ ] **Step 3: Write `desktop/src/hooks/useSettings.ts`**

```typescript
// Phase E: settings state hook
export function useSettings() {
  // TODO Phase E
}
```

- [ ] **Step 4: Write `desktop/src/hooks/useOllamaStatus.ts`**

```typescript
// Phase E: Ollama availability hook
export function useOllamaStatus() {
  // TODO Phase E
}
```

- [ ] **Step 5: Create placeholder files**

```bash
touch desktop/src/utils/.gitkeep desktop/src/components/.gitkeep
```

- [ ] **Step 6: Commit**

```bash
cd desktop && git add src/hooks/ src/utils/ src/components/ && git commit -m "chore: add Phase E hook stubs and component/utils dirs"
```

---

## Task 16: Rust scaffold

**Files:** All `desktop/src-tauri/` files listed in the File Map above.

- [ ] **Step 1: Write `desktop/src-tauri/Cargo.toml`**

```toml
[package]
name = "whatthefile"
version = "0.1.0"
description = "WhatTheFile — local-first semantic file search"
authors = []
edition = "2021"

[lib]
name = "whatthefile_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
```

- [ ] **Step 2: Write `desktop/src-tauri/build.rs`**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 3: Write `desktop/src-tauri/tauri.conf.json`**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "WhatTheFile",
  "version": "0.1.0",
  "identifier": "com.whatthefile.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "WhatTheFile",
        "width": 800,
        "height": 600,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

- [ ] **Step 4: Write `desktop/src-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    whatthefile_lib::run();
}
```

- [ ] **Step 5: Write `desktop/src-tauri/src/errors.rs`**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(String),
    #[error("indexer error: {0}")]
    Indexer(String),
    #[error("extractor error: {0}")]
    Extractor(String),
    #[error("LLM error: {0}")]
    Llm(String),
    #[error("search error: {0}")]
    Search(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
```

- [ ] **Step 6: Write `desktop/src-tauri/src/lib.rs`**

```rust
mod config;
mod db;
mod errors;
mod extractor;
mod chunker;
mod indexer;
mod llm;
mod platform;
#[path = "search.rs"]
mod search_engine;

use serde_json::Value;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            search,
            start_indexing,
            add_root,
            open_file,
            get_runtime_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn search(query: String) -> Result<Value, String> {
    todo!("Phase D: implement search command")
}

#[tauri::command]
async fn start_indexing(root_path: String) -> Result<(), String> {
    todo!("Phase B: implement start_indexing command")
}

#[tauri::command]
async fn add_root(path: String) -> Result<(), String> {
    todo!("Phase B: implement add_root command")
}

#[tauri::command]
async fn open_file(path: String) -> Result<(), String> {
    todo!("Phase E: implement open_file command")
}

#[tauri::command]
async fn get_runtime_status() -> Result<Value, String> {
    todo!("Phase D: implement get_runtime_status command")
}
```

- [ ] **Step 7: Write `desktop/src-tauri/src/db.rs`**

```rust
use crate::errors::AppError;

pub async fn init_db(db_path: &str) -> Result<(), AppError> {
    todo!("Phase D: initialize SQLite + sqlite-vec schema")
}

pub async fn upsert_file(path: &str, fingerprint: &str) -> Result<i64, AppError> {
    todo!("Phase D: upsert file record")
}

pub async fn delete_file(path: &str) -> Result<(), AppError> {
    todo!("Phase D: delete file and its chunks")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
```

- [ ] **Step 8: Write `desktop/src-tauri/src/indexer.rs`**

```rust
use crate::errors::AppError;

pub async fn run_index_job(root_path: &str) -> Result<(), AppError> {
    todo!("Phase B: implement two-phase incremental scan")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
```

- [ ] **Step 9: Write `desktop/src-tauri/src/extractor.rs`**

```rust
use crate::errors::AppError;

pub async fn extract_text(path: &str) -> Result<String, AppError> {
    todo!("Phase C: implement per-type text extraction")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
```

- [ ] **Step 10: Write `desktop/src-tauri/src/chunker.rs`**

```rust
pub struct Chunk {
    pub start_token: usize,
    pub end_token: usize,
    pub text: String,
}

pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<Chunk> {
    todo!("Phase C: implement 256/32 sliding window chunker")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
```

- [ ] **Step 11: Write `desktop/src-tauri/src/search.rs`**

```rust
use crate::errors::AppError;
use serde_json::Value;

pub async fn search_files(query: &str, limit: u32, offset: u32) -> Result<Value, AppError> {
    todo!("Phase D: FTS5/BM25 + sqlite-vec cosine + RRF blend")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
```

- [ ] **Step 12: Write `desktop/src-tauri/src/config.rs`**

```rust
use crate::errors::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub indexed_roots: Vec<String>,
}

pub fn load_config() -> Result<AppConfig, AppError> {
    todo!("Phase B: load config from app data dir")
}

pub fn save_config(config: &AppConfig) -> Result<(), AppError> {
    todo!("Phase B: persist config to app data dir")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
```

- [ ] **Step 13: Write `desktop/src-tauri/src/llm/mod.rs`**

```rust
pub mod embeddings;
pub mod runtime;
pub mod vision;
```

- [ ] **Step 14: Write `desktop/src-tauri/src/llm/embeddings.rs`**

```rust
use crate::errors::AppError;

pub async fn embed_text(text: &str) -> Result<Vec<f32>, AppError> {
    todo!("Phase D: call Ollama nomic-embed-text, return 64-dim Vec<f32>")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
```

- [ ] **Step 15: Write `desktop/src-tauri/src/llm/vision.rs`**

```rust
use crate::errors::AppError;

pub async fn describe_image(image_path: &str) -> Result<String, AppError> {
    todo!("Phase C: call Ollama qwen2.5vl:7b (fallback llava:7b), return text description")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
```

- [ ] **Step 16: Write `desktop/src-tauri/src/llm/runtime.rs`**

```rust
use crate::errors::AppError;

pub async fn ensure_model_loaded(model: &str) -> Result<(), AppError> {
    todo!("Phase D: check Ollama, pull model if needed, enforce single model lease")
}

pub async fn unload_model(model: &str) -> Result<(), AppError> {
    todo!("Phase D: explicitly release model from Ollama memory")
}

pub async fn release_stale_models() -> Result<(), AppError> {
    todo!("Phase D: on startup, detect and release models from previous crash")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
```

- [ ] **Step 17: Write `desktop/src-tauri/src/platform/mod.rs`**

```rust
use crate::errors::AppError;
use std::path::PathBuf;

pub fn app_data_dir() -> Result<PathBuf, AppError> {
    todo!("platform-specific: ~/Library/Application Support/WhatTheFile (mac) or %LOCALAPPDATA%\\WhatTheFile (win)")
}

pub fn copy_to_clipboard(text: &str) -> Result<(), AppError> {
    todo!("platform-specific clipboard write")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
```

- [ ] **Step 18: Create empty placeholder files**

```bash
touch desktop/src-tauri/capabilities/.gitkeep
touch desktop/src-tauri/icons/.gitkeep
touch desktop/src-tauri/migrations/.gitkeep
```

- [ ] **Step 19: Commit**

```bash
git add desktop/src-tauri/ && git commit -m "chore: add Rust scaffold with compilable todo!() stubs"
```

---

## Task 17: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Write `.github/workflows/ci.yml`**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  typecheck:
    name: TypeScript type check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
          cache-dependency-path: desktop/package-lock.json
      - run: npm install
        working-directory: desktop
      - run: npx tsc --noEmit
        working-directory: desktop

  test:
    name: Vitest unit tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
          cache-dependency-path: desktop/package-lock.json
      - run: npm install
        working-directory: desktop
      - run: npm test
        working-directory: desktop
```

- [ ] **Step 2: Commit**

```bash
git add .github/ && git commit -m "ci: add GitHub Actions workflow (typecheck + vitest)"
```

---

## Task 18: Final verification

- [ ] **Step 1: Run full test suite**

```bash
cd desktop && npm test
```

Expected:
```
Test Files  11 passed (11)
```

- [ ] **Step 2: TypeScript strict check**

```bash
cd desktop && npx tsc --noEmit
```

Expected: no output (zero errors).

- [ ] **Step 3: Confirm shared/ and scripts/ exist**

```bash
ls shared/ scripts/
```

Expected: both directories exist (`.gitkeep` inside).

- [ ] **Step 4: Confirm Rust stub files are all present**

```bash
find desktop/src-tauri/src -name "*.rs" | sort
```

Expected:
```
desktop/src-tauri/src/chunker.rs
desktop/src-tauri/src/config.rs
desktop/src-tauri/src/db.rs
desktop/src-tauri/src/errors.rs
desktop/src-tauri/src/extractor.rs
desktop/src-tauri/src/indexer.rs
desktop/src-tauri/src/lib.rs
desktop/src-tauri/src/llm/embeddings.rs
desktop/src-tauri/src/llm/mod.rs
desktop/src-tauri/src/llm/runtime.rs
desktop/src-tauri/src/llm/vision.rs
desktop/src-tauri/src/main.rs
desktop/src-tauri/src/platform/mod.rs
desktop/src-tauri/src/search.rs
```

- [ ] **Step 5: Update `CLAUDE.md` commands section**

The root `CLAUDE.md` references commands that now must be run from `desktop/`. Find the Commands section and update it:

```markdown
## Commands

All commands run from the `desktop/` directory:

```bash
cd desktop
npm install          # install dependencies
npm test             # run all tests (vitest, single run)
npm run test:watch   # run tests in watch mode
```

Run a single test file:
```bash
cd desktop && npx vitest run test/queryParser.test.ts
```

TypeScript type-check (no emit):
```bash
cd desktop && npx tsc --noEmit
```
```

- [ ] **Step 6: Final commit**

```bash
git add . && git commit -m "chore: Phase A complete — all tests passing, full scaffold in place"
```

---

## Definition of Done

- [ ] `npm test` (from `desktop/`) — 11 test suites, all green
- [ ] `npx tsc --noEmit` (from `desktop/`) — zero TypeScript errors
- [ ] All 10 `src/core/` modules exist with implementations matching test contracts
- [ ] `desktop/src-tauri/` contains valid Rust stubs — no `.unwrap()` in non-test code
- [ ] `.github/workflows/ci.yml` triggers on push and runs typecheck + tests
- [ ] `shared/`, `scripts/` directories exist at repo root
