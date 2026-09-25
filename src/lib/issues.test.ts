import { describe, expect, it, vi } from 'vitest';
import type { InvokeFn } from './app-info';
import {
  getMatchReview,
  listIssues,
  type IssuePage,
  type MatchReview,
} from './issues';

const issuePage: IssuePage = {
  items: [],
  total: 0,
  offset: 0,
  limit: 100,
  counts: {
    matchReview: 0,
    missingLocalFile: 0,
    inaccessibleCollection: 0,
    invalidLocalFile: 0,
    acquisitionFailed: 0,
  },
};

const review: MatchReview = {
  source: {
    id: 7,
    title: 'Song',
    artists: ['Artist'],
    album: 'Album',
    isrc: null,
    durationMs: 180_000,
    discNumber: 1,
    trackNumber: 3,
    explicit: false,
    versionKind: null,
    versionDetail: null,
  },
  outcome: 'review',
  selectedLibraryTrackId: null,
  method: null,
  confidence: 9_200,
  rejectedLibraryTrackIds: [],
  candidates: [],
};

describe('issue commands', () => {
  it('loads paginated issue projections', async () => {
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        expect(command).toBe('list_issues');
        expect(args).toEqual({ offset: 20, limit: 10 });
        return issuePage as T;
      },
    ) as InvokeFn;

    await expect(listIssues(20, 10, invoke)).resolves.toEqual(issuePage);
  });

  it('loads source and candidate context for match review', async () => {
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        expect(command).toBe('get_match_review');
        expect(args).toEqual({ sourceTrackId: 7 });
        return review as T;
      },
    ) as InvokeFn;

    await expect(getMatchReview(7, invoke)).resolves.toEqual(review);
  });
});
