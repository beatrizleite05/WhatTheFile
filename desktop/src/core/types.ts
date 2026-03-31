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
