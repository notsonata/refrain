import { describe, expect, it, vi } from 'vitest';
import type { InvokeFn } from './app-info';
import {
  clearMatchDecision,
  confirmMatch,
  getMatchCandidates,
  rejectMatch,
  type MatchResult,
} from './matching';

const result: MatchResult = {
  sourceTrackId: 7,
  outcome: 'review',
  selectedLibraryTrackId: null,
  method: null,
  confidence: 9_500,
  candidates: [],
  rejectedLibraryTrackIds: [],
};

describe('matching commands', () => {
  it('loads match evidence for a source track', async () => {
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        expect(command).toBe('get_match_candidates');
        expect(args).toEqual({ sourceTrackId: 7 });
        return result as T;
      },
    ) as InvokeFn;

    await expect(getMatchCandidates(7, invoke)).resolves.toEqual(result);
  });

  it('persists confirm, reject, and clear decisions', async () => {
    const invoke = vi.fn(async <T>() => undefined as T) as InvokeFn;

    await confirmMatch(7, 11, invoke);
    await rejectMatch(7, 12, 'wrong version', invoke);
    await clearMatchDecision(7, 12, invoke);

    expect(invoke).toHaveBeenNthCalledWith(1, 'confirm_match', {
      sourceTrackId: 7,
      libraryTrackId: 11,
    });
    expect(invoke).toHaveBeenNthCalledWith(2, 'reject_match', {
      sourceTrackId: 7,
      libraryTrackId: 12,
      reason: 'wrong version',
    });
    expect(invoke).toHaveBeenNthCalledWith(3, 'clear_match_decision', {
      sourceTrackId: 7,
      libraryTrackId: 12,
    });
  });
});
