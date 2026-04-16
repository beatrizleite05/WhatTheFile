import { describe, it, expect, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { getRecentFiles } from '../../src/api/files';

const mockInvoke = vi.mocked(invoke);

describe('files api', () => {
  it('invokes get_recent_files with provided limit', async () => {
    mockInvoke.mockResolvedValue([]);
    await getRecentFiles(4);
    expect(mockInvoke).toHaveBeenCalledWith('get_recent_files', { limit: 4 });
  });

  it('uses default limit when omitted', async () => {
    mockInvoke.mockResolvedValue([]);
    await getRecentFiles();
    expect(mockInvoke).toHaveBeenCalledWith('get_recent_files', { limit: 4 });
  });
});
