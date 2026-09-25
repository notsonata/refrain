import { describe, expect, it, vi } from 'vitest';
import type { InvokeFn } from './app-info';
import { listAcquisitionJobs, type AcquisitionJobPage } from './acquisition';

describe('acquisition commands', () => {
  it('loads the acquisition status projection', async () => {
    const page: AcquisitionJobPage = {
      items: [],
      total: 0,
      offset: 20,
      limit: 10,
    };
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        expect(command).toBe('list_acquisition_jobs');
        expect(args).toEqual({ offset: 20, limit: 10 });
        return page as T;
      },
    ) as InvokeFn;

    await expect(listAcquisitionJobs(20, 10, invoke)).resolves.toEqual(page);
  });
});
