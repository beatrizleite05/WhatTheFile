import type { FsEvent } from './types';

export function coalesceEvents(events: FsEvent[]): FsEvent[] {
  // Map preserves insertion order of first-seen keys.
  // Each subsequent event for the same path overwrites the previous.
  const latest = new Map<string, FsEvent>();
  for (const event of events) {
    latest.set(event.path, event);
  }
  return Array.from(latest.values());
}
