import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';

export type MatchOutcome = 'automatic' | 'review' | 'unresolved';
export type MatchRelationship = 'same' | 'different' | 'uncertain';

export interface MatchScoreBreakdown {
  title: number;
  artists: number;
  duration: number;
  album: number;
  trackDisc: number;
  total: number;
}

export interface MatchCandidateEvidence {
  libraryTrackId: number;
  relationship: MatchRelationship;
  score: MatchScoreBreakdown;
  exactIsrc: boolean;
  durationDifferenceMs: number | null;
  warnings: string[];
  incompatibilities: string[];
}

export interface MatchResult {
  sourceTrackId: number;
  outcome: MatchOutcome;
  selectedLibraryTrackId: number | null;
  method: string | null;
  confidence: number | null;
  candidates: MatchCandidateEvidence[];
  rejectedLibraryTrackIds: number[];
}

export function getMatchCandidates(
  sourceTrackId: number,
  invokeFn: InvokeFn = invoke,
): Promise<MatchResult> {
  return invokeFn<MatchResult>('get_match_candidates', { sourceTrackId });
}

export function confirmMatch(
  sourceTrackId: number,
  libraryTrackId: number,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('confirm_match', { sourceTrackId, libraryTrackId });
}

export function rejectMatch(
  sourceTrackId: number,
  libraryTrackId: number,
  reason: string | null = null,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('reject_match', {
    sourceTrackId,
    libraryTrackId,
    reason,
  });
}

export function clearMatchDecision(
  sourceTrackId: number,
  libraryTrackId: number | null = null,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('clear_match_decision', {
    sourceTrackId,
    libraryTrackId,
  });
}
