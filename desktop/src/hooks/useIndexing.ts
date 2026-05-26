// Re-export everything from IndexingContext so existing import paths don't change.
export { useIndexing, IndexingContext, IndexingProvider } from './IndexingContext';
export type { IndexingJob, IndexingContextValue } from './IndexingContext';
// Backward-compat alias — IndexingContextValue was previously named UseIndexingReturn.
export type { IndexingContextValue as UseIndexingReturn } from './IndexingContext';
