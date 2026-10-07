import { describe, expect, it, vi } from 'vitest';
import type { InvokeFn } from './app-info';
import {
  cancelActiveStaging,
  cancelStagingTrack,
  continueStagingTracks,
  excludeStagingTrackFromTracking,
  listStagingItems,
  rejectStagingCandidate,
  resetAcquisitionSession,
  resolveStagingTrack,
  retryFailedStaging,
  searchAgainStagingTrack,
  startAllStaging,
  startStagingTrack,
  type StagingPage,
} from './staging';

const page: StagingPage = {
  items: [],
  total: 0,
  offset: 0,
  limit: 100,
};

describe('staging commands', () => {
  it('loads the tracked acquisition workspace', async () => {
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        expect(command).toBe('list_staging_items');
        expect(args).toEqual({ offset: 20, limit: 10 });
        return page as T;
      },
    ) as InvokeFn;

    await expect(listStagingItems(20, 10, invoke)).resolves.toEqual(page);
  });

  it('maps per-track and bulk controls to native commands', async () => {
    const invokeMock = vi.fn(async () => undefined);
    const invoke = invokeMock as InvokeFn;

    await startStagingTrack(7, invoke);
    await cancelStagingTrack(12, invoke);
    await resolveStagingTrack(12, 'monochrome', 'candidate-a', invoke);
    await rejectStagingCandidate(12, 'sockseek', 'candidate-b', invoke);
    await searchAgainStagingTrack(12, invoke);
    await startAllStaging(invoke);
    await cancelActiveStaging(invoke);
    await resetAcquisitionSession(invoke);
    await retryFailedStaging(invoke);
    await continueStagingTracks([12, 14], invoke);
    await excludeStagingTrackFromTracking(7, invoke);

    expect(invokeMock.mock.calls).toEqual([
      ['start_staging_track', { libraryTrackId: 7 }],
      ['cancel_staging_track', { jobId: 12 }],
      [
        'resolve_staging_track',
        { jobId: 12, provider: 'monochrome', providerToken: 'candidate-a' },
      ],
      [
        'reject_staging_candidate',
        { jobId: 12, provider: 'sockseek', providerToken: 'candidate-b' },
      ],
      ['search_again_staging_track', { jobId: 12 }],
      ['start_all_staging'],
      ['cancel_active_staging'],
      ['reset_acquisition_session'],
      ['retry_failed_staging'],
      ['continue_staging_tracks', { jobIds: [12, 14] }],
      ['exclude_staging_track_from_tracking', { libraryTrackId: 7 }],
    ]);
  });
});
