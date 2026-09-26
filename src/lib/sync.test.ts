import { describe, expect, it, vi } from 'vitest';
import type { InvokeFn } from './app-info';
import {
  cancelSync,
  getSyncRun,
  listSyncRuns,
  startSync,
  type SyncRun,
} from './sync';

const run: SyncRun = {
  id: 14,
  scope: 'spotify',
  trigger: 'manual',
  status: 'succeeded',
  phase: 'complete',
  startedAt: 1_790_000_000_000,
  finishedAt: 1_790_000_001_000,
  sourceAdded: 0,
  sourceRemoved: 0,
  matched: 12,
  missing: 3,
  needsReview: 1,
  acquisitionFailed: 0,
  errorMessage: null,
};

describe('sync commands', () => {
  it('starts and cancels reconciliation through use-case commands', async () => {
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        if (command === 'start_sync') {
          expect(args).toEqual({ trigger: 'manual' });
          return run as T;
        }
        expect(command).toBe('cancel_sync');
        return true as T;
      },
    ) as InvokeFn;

    await expect(startSync('manual', invoke)).resolves.toEqual(run);
    await expect(cancelSync(invoke)).resolves.toBe(true);
  });

  it('loads one run and paginated run history', async () => {
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        if (command === 'get_sync_run') {
          expect(args).toEqual({ runId: 14 });
          return run as T;
        }
        expect(command).toBe('list_sync_runs');
        expect(args).toEqual({ scope: null, offset: 20, limit: 10 });
        return { items: [run], total: 21, offset: 20, limit: 10 } as T;
      },
    ) as InvokeFn;

    await expect(getSyncRun(14, invoke)).resolves.toEqual(run);
    await expect(listSyncRuns(null, 20, 10, invoke)).resolves.toEqual({
      items: [run],
      total: 21,
      offset: 20,
      limit: 10,
    });
  });
});
