// Drives the eval harness:
//  1. Builds + runs the `eval` Rust binary (which indexes the corpus and runs queries).
//  2. Reads its JSON output from stdout.
//  3. Computes Recall@10 / MRR / NDCG@10 per query using src/core/metrics.ts.
//  4. Writes test/fixtures/eval-results/<YYYY-MM-DD>-<sha>.json.
//
// Run from desktop/: `npm run eval` (cwd = desktop/).

import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { recallAtK, mrr, ndcgAtK } from '../src/core/metrics.js';

interface QueryResult {
  query: string;
  expectedFiles: string[];
  retrieved: string[];
  scores: number[];
  group?: string;
  category?: string;
}

interface EvalOutput {
  runMeta: {
    sha: string;
    date: string;
    corpusPath: string;
    ollamaUrl: string;
    queriesPath: string;
  };
  queries: QueryResult[];
}

interface PerQueryMetrics extends QueryResult {
  recallAt10: number;
  mrr: number;
  ndcgAt10: number;
}

interface Aggregate {
  recallAt10: number;
  mrr: number;
  ndcgAt10: number;
  numQueries: number;
}

function shaShort(): string {
  try {
    return execFileSync('git', ['rev-parse', '--short', 'HEAD'], { encoding: 'utf8' }).trim();
  } catch {
    return 'nogit';
  }
}

function isoDate(): string {
  return new Date().toISOString().slice(0, 10);
}

function aggregateMetrics(rows: PerQueryMetrics[]): Aggregate {
  if (rows.length === 0) {
    return { recallAt10: 0, mrr: 0, ndcgAt10: 0, numQueries: 0 };
  }
  const sum = (f: (r: PerQueryMetrics) => number) =>
    rows.reduce((acc, r) => acc + f(r), 0) / rows.length;
  return {
    recallAt10: sum((r) => r.recallAt10),
    mrr: sum((r) => r.mrr),
    ndcgAt10: sum((r) => r.ndcgAt10),
    numQueries: rows.length,
  };
}

function main(): void {
  const sha = shaShort();
  const date = isoDate();

  const cargoArgs = [
    'run',
    '--release',
    '--bin',
    'eval',
    '--quiet',
    '--',
    '--queries',
    '../test/fixtures/eval-queries.json',
    '--sha',
    sha,
    '--date',
    date,
  ];

  // Allow reusing an existing indexed DB to skip the slow indexing phase.
  // Set EVAL_DB=/path/to/db.sqlite EVAL_SKIP_INDEX=1 npm run eval
  const evalDb = process.env['EVAL_DB'];
  const skipIndex = process.env['EVAL_SKIP_INDEX'] === '1';
  if (evalDb) { cargoArgs.push('--db', evalDb); }
  if (skipIndex) { cargoArgs.push('--skip-index'); }

  console.error(`eval: cargo ${cargoArgs.join(' ')}`);
  const stdout = execFileSync('cargo', cargoArgs, {
    cwd: resolve('src-tauri'),
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'inherit'],
    maxBuffer: 64 * 1024 * 1024,
  });

  const parsed = JSON.parse(stdout) as EvalOutput;

  // Per-query metrics. expectedFiles get uniform graded relevance of 3
  // until we have actual graded labels.
  const perQuery: PerQueryMetrics[] = parsed.queries.map((q) => {
    const graded: Record<string, number> = {};
    for (const f of q.expectedFiles) graded[f] = 3;
    const isNegative = q.expectedFiles.length === 0;
    return {
      ...q,
      // For negative cases, "recall" is undefined; report 0 as a neutral marker
      // (these queries don't contribute to recall improvements). MRR/NDCG return
      // 0 by definition when relevant set is empty.
      recallAt10: isNegative ? 0 : recallAtK(q.expectedFiles, q.retrieved, 10),
      mrr: mrr(q.expectedFiles, q.retrieved),
      ndcgAt10: ndcgAtK(graded, q.retrieved, 10),
    };
  });

  // Aggregate excludes negative queries — they have no "relevant" docs to recall.
  const positiveRows = perQuery.filter((r) => r.expectedFiles.length > 0);
  const aggregate = aggregateMetrics(positiveRows);

  // Group breakdown (also positive-only).
  const byGroup: Record<string, Aggregate> = {};
  for (const g of ['A', 'B', 'C']) {
    const rows = positiveRows.filter((r) => r.group === g);
    byGroup[g] = aggregateMetrics(rows);
  }

  const output = {
    runMeta: parsed.runMeta,
    perQuery,
    aggregate,
    byGroup,
  };

  const outDir = resolve('test/fixtures/eval-results');
  mkdirSync(outDir, { recursive: true });
  const outPath = resolve(outDir, `${date}-${sha}.json`);
  writeFileSync(outPath, JSON.stringify(output, null, 2));

  console.error(`eval: wrote ${outPath}`);
  console.error(
    `eval: Recall@10=${aggregate.recallAt10.toFixed(3)} MRR=${aggregate.mrr.toFixed(3)} NDCG@10=${aggregate.ndcgAt10.toFixed(3)} (n=${aggregate.numQueries})`,
  );
}

main();
