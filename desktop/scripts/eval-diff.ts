// Compares the most recent eval result to baseline.json.
// Usage: tsx scripts/eval-diff.ts [--group A|B|C]
//
// Prints a short table of metric / baseline / current / delta.
// Always exits 0 — gating logic is enforced separately by the rework commits.

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';

interface Aggregate {
  recallAt10: number;
  mrr: number;
  ndcgAt10: number;
  numQueries: number;
}

interface ResultFile {
  aggregate: Aggregate;
  byGroup: Record<string, Aggregate>;
  runMeta: { sha: string; date: string };
}

function findLatestResult(dir: string): string | null {
  if (!existsSync(dir)) return null;
  const files = readdirSync(dir)
    .filter((f) => f.endsWith('.json') && f !== 'baseline.json')
    .sort();
  return files.length === 0 ? null : resolve(dir, files[files.length - 1]);
}

function fmt(n: number): string {
  return n.toFixed(3);
}

function fmtDelta(curr: number, base: number): string {
  const d = curr - base;
  const sign = d >= 0 ? '+' : '';
  return `${sign}${d.toFixed(3)}`;
}

function main(): void {
  const argv = process.argv.slice(2);
  let group: string | null = null;
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--group' && i + 1 < argv.length) {
      group = argv[i + 1];
      i += 1;
    }
  }

  const resultsDir = resolve('test/fixtures/eval-results');
  const baselinePath = resolve(resultsDir, 'baseline.json');

  if (!existsSync(baselinePath)) {
    console.log('no baseline yet — run npm run eval:promote to set one');
    return;
  }

  const baselineRaw = JSON.parse(readFileSync(baselinePath, 'utf8'));
  if (baselineRaw && baselineRaw.placeholder === true) {
    console.log('no baseline yet — run npm run eval:promote to set one');
    return;
  }
  const baseline = baselineRaw as ResultFile;

  const latestPath = findLatestResult(resultsDir);
  if (!latestPath) {
    console.log('no current eval result found — run npm run eval first');
    return;
  }
  const current = JSON.parse(readFileSync(latestPath, 'utf8')) as ResultFile;

  const pickAgg = (r: ResultFile): Aggregate =>
    group ? r.byGroup[group] ?? { recallAt10: 0, mrr: 0, ndcgAt10: 0, numQueries: 0 } : r.aggregate;

  const b = pickAgg(baseline);
  const c = pickAgg(current);

  const label = group ? `group ${group}` : 'overall';
  console.log(`Comparing ${label}: baseline ${baseline.runMeta.sha} -> current ${current.runMeta.sha}`);
  console.log(`(n queries: baseline=${b.numQueries}, current=${c.numQueries})\n`);
  console.log('metric        baseline    current     delta');
  console.log('------        --------    -------     -----');
  console.log(`Recall@10     ${fmt(b.recallAt10)}       ${fmt(c.recallAt10)}       ${fmtDelta(c.recallAt10, b.recallAt10)}`);
  console.log(`MRR           ${fmt(b.mrr)}       ${fmt(c.mrr)}       ${fmtDelta(c.mrr, b.mrr)}`);
  console.log(`NDCG@10       ${fmt(b.ndcgAt10)}       ${fmt(c.ndcgAt10)}       ${fmtDelta(c.ndcgAt10, b.ndcgAt10)}`);
}

main();
