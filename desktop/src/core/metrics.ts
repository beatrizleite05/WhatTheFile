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
