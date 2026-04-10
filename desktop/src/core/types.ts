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
  /**
   * Search mode sent to the Rust backend.
   *
   * - `"hybrid"` (default): FTS5/BM25 + vector similarity blended with RRF.
   * - `"keyword"`: FTS5/BM25 only — no vector pass, no LLM fallback in the parser.
   * - `"semantic"`: vector similarity only.
   *
   * Maps directly to `SearchQuery.mode` in `search.rs`.
   */
  mode: 'hybrid' | 'keyword' | 'semantic';
  /**
   * Frontend-only UI hint — NOT sent to the Rust backend.
   *
   * Indicates how confident the deterministic parser is in the structured
   * fields it extracted (0 = likely needs LLM fallback, 1 = fully resolved).
   * The search bar uses this to decide whether to show the "AI-interpreted"
   * indicator on query pills.  Rust `search.rs` never reads this field.
   */
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
