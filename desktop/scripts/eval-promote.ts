// Promotes the most recent eval result to baseline.json and appends to HISTORY.md.
// Usage: tsx scripts/eval-promote.ts [--note "free-form context"]

import { copyFileSync, existsSync, readFileSync, readdirSync, writeFileSync, appendFileSync } from 'node:fs';
import { resolve } from 'node:path';

interface ResultFile {
  aggregate: { recallAt10: number; mrr: number; ndcgAt10: number; numQueries: number };
  runMeta: { sha: string; date: string };
}

function findLatestResult(dir: string): string | null {
  if (!existsSync(dir)) return null;
  const files = readdirSync(dir)
    .filter((f) => f.endsWith('.json') && f !== 'baseline.json')
    .sort();
  return files.length === 0 ? null : resolve(dir, files[files.length - 1]);
}

function main(): void {
  const argv = process.argv.slice(2);
  let note = '';
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--note' && i + 1 < argv.length) {
      note = argv[i + 1];
      i += 1;
    }
  }

  const resultsDir = resolve('test/fixtures/eval-results');
  const latest = findLatestResult(resultsDir);
  if (!latest) {
    console.error('no eval result found — run npm run eval first');
    process.exit(1);
  }

  const baselinePath = resolve(resultsDir, 'baseline.json');
  copyFileSync(latest, baselinePath);

  const r = JSON.parse(readFileSync(latest, 'utf8')) as ResultFile;
  const a = r.aggregate;
  const line = `${r.runMeta.date} ${r.runMeta.sha}: Recall@10=${a.recallAt10.toFixed(3)} MRR=${a.mrr.toFixed(3)} NDCG=${a.ndcgAt10.toFixed(3)} (n=${a.numQueries})${note ? `. ${note}` : ''}\n`;

  const historyPath = resolve(resultsDir, 'HISTORY.md');
  if (!existsSync(historyPath)) {
    writeFileSync(historyPath, '# Eval History\n\n');
  }
  appendFileSync(historyPath, line);

  console.log(`promoted ${latest} -> ${baselinePath}`);
  console.log(`appended to HISTORY.md: ${line.trim()}`);
}

main();
